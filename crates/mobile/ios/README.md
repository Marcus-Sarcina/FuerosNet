# iOS shell

Swift over `rhtn-ffi`. Nothing is built here yet.

It owes what the Android shell owes: the same obligations from
`light-client-requirements.md` §1.3, §5 and §6, and the same backup
statement from design §13.7.1. The catalogue carries them as PRD-01 to
PRD-05 and PRD-07 to PRD-09, once for the application rather than once per
platform.

**Where the platforms differ, the difference is the platform's and not the
protocol's.** iOS exposes UWB through its own framework and restricts NFC
more tightly than Android does; a channel the framework will not give an
application is unavailable, which is a value the ceremony already has a
name for.
