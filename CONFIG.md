# Configuration options

Cove's config file uses the [TOML](https://toml.io/) format.

## `data_dir`

**Required:** no  
**Type:** path  
**Default:** platform-dependent

The directory that cove stores its data in when not running in ephemeral
mode.

Relative paths are interpreted relative to the user's home directory.

See also the `--data-dir` command line option.

## `ephemeral`

**Required:** yes  
**Type:** boolean  
**Default:** `false`

Whether to start in ephemeral mode.

In ephemeral mode, cove doesn't store any data. It completely ignores
any options related to the data dir.

See also the `--ephemeral` command line option.

## `euph.rooms.<room>.autojoin`

**Required:** yes  
**Type:** boolean  
**Default:** `false`

Whether to automatically join this room on startup.

## `euph.rooms.<room>.force_username`

**Required:** yes  
**Type:** boolean  
**Default:** `false`

If `euph.rooms.<room>.username` is set, this will force cove to set the
username even if there is already a different username associated with
the current session.

## `euph.rooms.<room>.password`

**Required:** no  
**Type:** string

If set, cove will try once to use this password to authenticate, should
the room be password-protected.

## `euph.rooms.<room>.username`

**Required:** no  
**Type:** string

If set, cove will set this username upon joining if there is no username
associated with the current session.

## `offline`

**Required:** yes  
**Type:** boolean  
**Default:** `false`

Whether to start in offline mode.

In offline mode, cove won't automatically join rooms marked via the
`autojoin` option on startup. You can still join those rooms manually by
pressing `a` in the rooms list.

See also the `--offline` command line option.

## `rooms_sort_order`

**Required:** yes  
**Type:** string  
**Values:** `alphabet`, `importance`  
**Default:** `alphabet`

Initial sort order of rooms list.

`alphabet` sorts rooms in alphabetic order.

`importance` sorts rooms by the following criteria (in descending order
of priority):

1. connected rooms before unconnected rooms
2. rooms with unread messages before rooms without
3. alphabetic order
