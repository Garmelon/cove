- Determine room name
- Connect for the first time
  - If connection fails: Show error, done
  - Set room
    - If room is invalid: Show error, done
  - If no nick is set by default: Let user choose nick
  - Identify yourself
    - If nick is invalid: Show error, let user edit nick
    - If identity is invalid: Show error, done
  - Listen to events, send commands
- Reconnect
  - If connection fails: Show error, done
  - Set room
    - If room is invalid: Show error, done
  - Identify yourself
    - If nick is invalid: Show error, let user edit nick
    - If identity is invalid: Show error, done
  - Listen to events, send commands

General state:
- Initial nick (optional)
- A way to stop the entire room

State present when WS connection exists:
- Connection itself
- Next command id
- Replies

State present when fully connected:
- Own session
- Others
