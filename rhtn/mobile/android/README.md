# Android shell

Kotlin over `rhtn-ffi`. Nothing is built here yet.

**What this shell owes**, beyond rendering the client's states:

- The proximity channels the device has, and no promotion of one that
  failed (`light-client-requirements.md` §1.3). Android exposes UWB and
  NFC to an application; a device lacking one reports it as unavailable
  rather than absent from the list.
- A guided camera capture whose frames cross the boundary as pixels, with
  whatever the platform's pipeline attached stripped at the boundary (§1.3).
- The privacy choices §5 requires the user be able to make, and the
  warnings §6 requires before anything irreversible.
- Encrypted backup under a key kept apart from the backup, and a plain
  statement of what losing the device costs (design §13.7.1).

The catalogue carries these as PRD-01 to PRD-05 and PRD-07 to PRD-09.
