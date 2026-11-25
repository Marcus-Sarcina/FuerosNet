//! **The media face of the bench package: the other end of a live
//! session** (`resource-requirements.md` §3.3's media port) [author,
//! 2026-11-24, 2026-11-25].
//!
//! A live session is signalled through the ordinary path and carried
//! outside it: the page's offer arrives as one request of its own, the
//! answer goes back as that request's body, and the session itself runs
//! between the member's device and the one UDP port this package declared
//! in its manifest. This holds that port. It answers an offer, completes
//! the session, sends back whatever a channel says to it, and counts the
//! audio and video that arrive, so a bench run reads from this instance's
//! log whether a session was really made and what crossed it.
//!
//! **It keeps nothing that crossed**: a frame is counted and dropped.
//!
//! **One socket for every session**, since one port is what a package is
//! given. A datagram belongs to the session that accepts it; one that no
//! session accepts is said, with its sender and its size, and answered
//! with nothing — the first thirty-two, so a flood fills no log.

use std::io::Write;
use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use str0m::change::SdpOffer;
use str0m::media::{MediaKind, Mid};
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event, IceConnectionState, Input, Output, Rtc, RtcConfig};

/// The most sessions held at once: a bench has four phones.
const MOST: usize = 8;

/// The longest a session is held, whatever its peer does: past the
/// longest a `live` ask may run (`resource-requirements.md` §3.4).
const LONGEST: Duration = Duration::from_secs(150);

/// How often a session says what has crossed it.
const SAID_EVERY: Duration = Duration::from_secs(5);

/// One offer put to the thread that holds the port.
struct Asked {
    offer: String,
    at: SocketAddr,
    answer: mpsc::Sender<Result<String, String>>,
}

/// **The port, held**: what the request path answers an offer through.
pub struct Media {
    port: u16,
    asks: mpsc::Sender<Asked>,
}

impl Media {
    /// Hold `port` on every address of this host and serve sessions there
    /// until this is dropped and the last of them has ended. `port` 0 is
    /// whichever the host gives, for a test.
    pub fn start(port: u16) -> std::io::Result<Media> {
        // the provider this build carries, named once for the process
        static PROVIDER: std::sync::Once = std::sync::Once::new();
        PROVIDER.call_once(|| str0m::crypto::from_feature_flags().install_process_default());
        let socket = UdpSocket::bind(("0.0.0.0", port))?;
        let port = socket.local_addr()?.port();
        let (asks, asked) = mpsc::channel();
        std::thread::Builder::new()
            .name("media".into())
            .spawn(move || serve(&socket, &asked))?;
        say(&format!(
            "{{\"what\":\"stream.media.held\",\"port\":{port}}}"
        ));
        Ok(Media { port, asks })
    }

    /// The port held.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// **Answer `offer`** with a session whose one candidate is `at`: the
    /// address a member's device reaches this port by, which the host
    /// said and this package could not otherwise know. The answer's SDP,
    /// or why there is none.
    pub fn answer(&self, offer: &str, at: SocketAddr) -> Result<String, String> {
        let (answer, answered) = mpsc::channel();
        self.asks
            .send(Asked {
                offer: offer.to_string(),
                at,
                answer,
            })
            .map_err(|_| "the media port is no longer held".to_string())?;
        answered
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| "the media port did not answer".to_string())?
    }
}

/// One session, and what has crossed it.
struct Session {
    n: u32,
    rtc: Rtc,
    /// The address its candidate named: where its datagrams are for.
    at: SocketAddr,
    began: Instant,
    said: Instant,
    kinds: Vec<(Mid, MediaKind)>,
    audio: u64,
    audio_bytes: u64,
    video: u64,
    video_bytes: u64,
    echoed: u64,
    over: bool,
}

impl Session {
    /// One thing the session said of itself.
    fn saw(&mut self, event: Event) {
        let n = self.n;
        match event {
            Event::Connected => say(&format!(
                "{{\"what\":\"stream.session.connected\",\"n\":{n}}}"
            )),
            Event::IceConnectionStateChange(state) => {
                say(&format!(
                    "{{\"what\":\"stream.session.ice\",\"n\":{n},\"state\":\"{state:?}\"}}"
                ));
                if state == IceConnectionState::Disconnected {
                    self.over = true;
                }
            }
            Event::MediaAdded(m) => {
                say(&format!(
                    "{{\"what\":\"stream.session.media\",\"n\":{n},\"kind\":\"{}\",\"direction\":\"{:?}\"}}",
                    kind(m.kind),
                    m.direction
                ));
                self.kinds.push((m.mid, m.kind));
            }
            Event::MediaData(d) => {
                let which = self
                    .kinds
                    .iter()
                    .find(|(m, _)| *m == d.mid)
                    .map(|(_, k)| *k);
                let bytes = d.data.len() as u64;
                match which {
                    Some(MediaKind::Audio) => {
                        self.audio += 1;
                        self.audio_bytes += bytes;
                    }
                    Some(MediaKind::Video) => {
                        self.video += 1;
                        self.video_bytes += bytes;
                    }
                    None => {}
                }
            }
            Event::ChannelOpen(_, label) => {
                let plain: String = label
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
                    .take(32)
                    .collect();
                say(&format!(
                    "{{\"what\":\"stream.session.channel\",\"n\":{n},\"label\":\"{plain}\"}}"
                ));
            }
            // **what a channel says comes back to it**, which is all this
            // end has to say
            Event::ChannelData(d) => {
                if let Some(mut channel) = self.rtc.channel(d.id)
                    && matches!(channel.write(d.binary, &d.data), Ok(true))
                {
                    self.echoed += 1;
                }
            }
            _ => {}
        }
    }

    /// What has crossed, as one line.
    fn counts(&mut self, what: &str) {
        self.said = Instant::now();
        say(&format!(
            "{{\"what\":\"stream.session.{what}\",\"n\":{},\"seconds\":{},\"audio\":{},\"audio_bytes\":{},\"video\":{},\"video_bytes\":{},\"echoed\":{}}}",
            self.n,
            self.began.elapsed().as_secs(),
            self.audio,
            self.audio_bytes,
            self.video,
            self.video_bytes,
            self.echoed
        ));
    }
}

fn kind(k: MediaKind) -> &'static str {
    match k {
        MediaKind::Audio => "audio",
        MediaKind::Video => "video",
    }
}

/// One line of this instance's log.
fn say(line: &str) {
    println!("{line}");
    let _ = std::io::stdout().flush();
}

/// A session for `asked`, answered, or why not.
fn open(sessions: &mut Vec<Session>, next: &mut u32, asked: &Asked) -> Result<String, String> {
    if sessions.len() >= MOST {
        return Err(format!("{MOST} sessions are held, which is the most"));
    }
    let offer =
        SdpOffer::from_sdp_string(&asked.offer).map_err(|e| format!("not an offer: {e}"))?;
    // **the lite end**: one address, said once, and the other end does
    // the finding (RFC 8445 §2.5), which suits a port on a public host
    let mut rtc = RtcConfig::new().set_ice_lite(true).build(Instant::now());
    let candidate = Candidate::host(asked.at, "udp").map_err(|e| format!("{}: {e}", asked.at))?;
    rtc.add_local_candidate(candidate);
    let answer = rtc
        .sdp_api()
        .accept_offer(offer)
        .map_err(|e| format!("the offer was not accepted: {e}"))?;
    *next += 1;
    let now = Instant::now();
    say(&format!(
        "{{\"what\":\"stream.session.answered\",\"n\":{},\"at\":\"{}\"}}",
        *next, asked.at
    ));
    sessions.push(Session {
        n: *next,
        rtc,
        at: asked.at,
        began: now,
        said: now,
        kinds: Vec::new(),
        audio: 0,
        audio_bytes: 0,
        video: 0,
        video_bytes: 0,
        echoed: 0,
        over: false,
    });
    Ok(answer.to_sdp_string())
}

/// The thread that holds the port: offers taken, each session's output
/// sent and its events read, one datagram or one timeout taken in, and
/// round again.
fn serve(socket: &UdpSocket, asked: &mpsc::Receiver<Asked>) {
    let mut sessions: Vec<Session> = Vec::new();
    let mut next = 0u32;
    let mut strangers = 0u32;
    let mut closed = false;
    let mut buf = vec![0u8; 2000];
    loop {
        loop {
            match asked.try_recv() {
                Ok(a) => {
                    let answer = open(&mut sessions, &mut next, &a);
                    let _ = a.answer.send(answer);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    closed = true;
                    break;
                }
            }
        }
        if closed && sessions.is_empty() {
            return;
        }
        // an offer waits no longer than this for the socket's next read
        let mut wake = Instant::now() + Duration::from_millis(50);
        for s in &mut sessions {
            loop {
                match s.rtc.poll_output() {
                    Ok(Output::Timeout(at)) => {
                        wake = wake.min(at);
                        break;
                    }
                    Ok(Output::Transmit(t)) => {
                        let _ = socket.send_to(&t.contents, t.destination);
                    }
                    Ok(Output::Event(e)) => s.saw(e),
                    Err(e) => {
                        say(&format!(
                            "{{\"what\":\"stream.session.failed\",\"n\":{},\"why\":\"{}\"}}",
                            s.n,
                            plain(&e.to_string())
                        ));
                        s.over = true;
                        break;
                    }
                }
            }
            if s.said.elapsed() >= SAID_EVERY && !s.over {
                s.counts("counts");
            }
        }
        sessions.retain_mut(|s| {
            let held = !s.over && s.rtc.is_alive() && s.began.elapsed() < LONGEST;
            if !held {
                s.counts("ended");
            }
            held
        });
        let wait = wake
            .saturating_duration_since(Instant::now())
            .max(Duration::from_millis(1));
        if socket.set_read_timeout(Some(wait)).is_err() {
            return;
        }
        match socket.recv_from(&mut buf) {
            Ok((n, source)) => {
                let now = Instant::now();
                let mut taken = false;
                for s in &mut sessions {
                    let Ok(contents) = buf[..n].try_into() else {
                        break;
                    };
                    let input = Input::Receive(
                        now,
                        Receive {
                            proto: Protocol::Udp,
                            source,
                            destination: s.at,
                            contents,
                        },
                    );
                    if s.rtc.accepts(&input) {
                        if s.rtc.handle_input(input).is_err() {
                            s.over = true;
                        }
                        taken = true;
                        break;
                    }
                }
                if !taken && strangers < 32 {
                    strangers += 1;
                    // RFC 8489: a binding request is type 0x0001 under the
                    // magic cookie
                    let stun =
                        n >= 20 && buf[..2] == [0, 1] && buf[4..8] == [0x21, 0x12, 0xa4, 0x42];
                    say(&format!(
                        "{{\"what\":\"stream.datagram\",\"from\":\"{source}\",\"bytes\":{n},\"stun\":{stun}}}"
                    ));
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => return,
        }
        let now = Instant::now();
        for s in &mut sessions {
            if s.rtc.handle_input(Input::Timeout(now)).is_err() {
                s.over = true;
            }
        }
    }
}

/// Words fit for one JSON string of this log.
fn plain(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() && *c != '"' && *c != '\\')
        .take(200)
        .collect()
}
