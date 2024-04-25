# Config file format

Cove's config file uses the [TOML](https://toml.io/) format.

Here is an example config that changes a few different options:

```toml
measure_widths = true
rooms_sort_order = "importance"

[euph.servers."euphoria.leet.nu".rooms]
welcome.autojoin = true
test.username = "badingle"
test.force_username = true
private.password = "foobar"

[keys]
general.abort = ["esc", "ctrl+c"]
general.exit = "ctrl+q"
tree.action.fold_tree = "f"
```

## Key bindings

Key bindings are specified as strings or lists of strings. Each string specifies
a main key and zero or more modifier keys. The modifier keys (if any) are listed
first, followed by the main key. They are separated by the `+` character and
**no** whitespace.

Examples of key bindings:
- `"ctrl+c"`
- `"X"` (not `"shift+x"`)
- `"space"` or `" "` (both space bar)
- `["g", "home"]`
- `["K", "ctrl+up"]`
- `["f1", "?"]`
- `"ctrl+alt+f3"`
- `["enter", "any+enter"]` (matches `enter` regardless of modifiers)

Available main keys:
- Any single character that can be typed
- `esc`, `enter`, `space`, `tab`, `backtab`
- `backspace`, `delete`, `insert`
- `left`, `right`, `up`, `down`
- `home`, `end`, `pageup`, `pagedown`
- `f1`, `f2`, ...

Available modifiers:
- `shift` (must not be used with single characters)
- `ctrl`
- `alt`
- `any` (matches as long as at least one modifier is pressed)

## Available options

### `data_dir`

**Required:** no  
**Type:** path  
**Default:** platform-dependent

The directory that cove stores its data in when not running in ephemeral
mode.

Relative paths are interpreted relative to the user's home directory.

See also the `--data-dir` command line option.

### `ephemeral`

**Required:** yes  
**Type:** boolean  
**Default:** `false`

Whether to start in ephemeral mode.

In ephemeral mode, cove doesn't store any data. It completely ignores
any options related to the data dir.

See also the `--ephemeral` command line option.

### `euph.servers.<domain>.rooms.<room>.autojoin`

**Required:** yes  
**Type:** boolean  
**Default:** `false`

Whether to automatically join this room on startup.

### `euph.servers.<domain>.rooms.<room>.force_username`

**Required:** yes  
**Type:** boolean  
**Default:** `false`

If `euph.rooms.<room>.username` is set, this will force cove to set the
username even if there is already a different username associated with
the current session.

### `euph.servers.<domain>.rooms.<room>.password`

**Required:** no  
**Type:** string

If set, cove will try once to use this password to authenticate, should
the room be password-protected.

### `euph.servers.<domain>.rooms.<room>.username`

**Required:** no  
**Type:** string

If set, cove will set this username upon joining if there is no username
associated with the current session.

### `keys.cursor.down`

**Required:** yes  
**Type:** key binding  
**Default:** `["j", "down"]`

Move down.

### `keys.cursor.to_bottom`

**Required:** yes  
**Type:** key binding  
**Default:** `["G", "end"]`

Move to bottom.

### `keys.cursor.to_top`

**Required:** yes  
**Type:** key binding  
**Default:** `["g", "home"]`

Move to top.

### `keys.cursor.up`

**Required:** yes  
**Type:** key binding  
**Default:** `["k", "up"]`

Move up.

### `keys.editor.action.backspace`

**Required:** yes  
**Type:** key binding  
**Default:** `["ctrl+h", "backspace"]`

Delete before cursor.

### `keys.editor.action.clear`

**Required:** yes  
**Type:** key binding  
**Default:** `"ctrl+l"`

Clear editor contents.

### `keys.editor.action.delete`

**Required:** yes  
**Type:** key binding  
**Default:** `["ctrl+d", "delete"]`

Delete after cursor.

### `keys.editor.action.external`

**Required:** yes  
**Type:** key binding  
**Default:** `["ctrl+x", "alt+e"]`

Edit in external editor.

### `keys.editor.cursor.down`

**Required:** yes  
**Type:** key binding  
**Default:** `"down"`

Move down.

### `keys.editor.cursor.end`

**Required:** yes  
**Type:** key binding  
**Default:** `["ctrl+e", "end"]`

Move to end of line.

### `keys.editor.cursor.left`

**Required:** yes  
**Type:** key binding  
**Default:** `["ctrl+b", "left"]`

Move left.

### `keys.editor.cursor.left_word`

**Required:** yes  
**Type:** key binding  
**Default:** `["alt+b", "ctrl+left"]`

Move left a word.

### `keys.editor.cursor.right`

**Required:** yes  
**Type:** key binding  
**Default:** `["ctrl+f", "right"]`

Move right.

### `keys.editor.cursor.right_word`

**Required:** yes  
**Type:** key binding  
**Default:** `["alt+f", "ctrl+right"]`

Move right a word.

### `keys.editor.cursor.start`

**Required:** yes  
**Type:** key binding  
**Default:** `["ctrl+a", "home"]`

Move to start of line.

### `keys.editor.cursor.up`

**Required:** yes  
**Type:** key binding  
**Default:** `"up"`

Move up.

### `keys.general.abort`

**Required:** yes  
**Type:** key binding  
**Default:** `"esc"`

Abort/close.

### `keys.general.confirm`

**Required:** yes  
**Type:** key binding  
**Default:** `"enter"`

Confirm.

### `keys.general.exit`

**Required:** yes  
**Type:** key binding  
**Default:** `"ctrl+c"`

Quit cove.

### `keys.general.focus`

**Required:** yes  
**Type:** key binding  
**Default:** `"tab"`

Advance focus.

### `keys.general.help`

**Required:** yes  
**Type:** key binding  
**Default:** `"f1"`

Show this help.

### `keys.general.log`

**Required:** yes  
**Type:** key binding  
**Default:** `"f12"`

Show log.

### `keys.room.action.account`

**Required:** yes  
**Type:** key binding  
**Default:** `"A"`

Manage account.

### `keys.room.action.authenticate`

**Required:** yes  
**Type:** key binding  
**Default:** `"a"`

Authenticate.

### `keys.room.action.more_messages`

**Required:** yes  
**Type:** key binding  
**Default:** `"m"`

Download more messages.

### `keys.room.action.nick`

**Required:** yes  
**Type:** key binding  
**Default:** `"n"`

Change nick.

### `keys.rooms.action.change_sort_order`

**Required:** yes  
**Type:** key binding  
**Default:** `"s"`

Change sort order.

### `keys.rooms.action.connect`

**Required:** yes  
**Type:** key binding  
**Default:** `"c"`

Connect to selected room.

### `keys.rooms.action.connect_all`

**Required:** yes  
**Type:** key binding  
**Default:** `"C"`

Connect to all rooms.

### `keys.rooms.action.connect_autojoin`

**Required:** yes  
**Type:** key binding  
**Default:** `"a"`

Connect to all autojoin rooms.

### `keys.rooms.action.delete`

**Required:** yes  
**Type:** key binding  
**Default:** `"X"`

Delete room.

### `keys.rooms.action.disconnect`

**Required:** yes  
**Type:** key binding  
**Default:** `"d"`

Disconnect from selected room.

### `keys.rooms.action.disconnect_all`

**Required:** yes  
**Type:** key binding  
**Default:** `"D"`

Disconnect from all rooms.

### `keys.rooms.action.disconnect_non_autojoin`

**Required:** yes  
**Type:** key binding  
**Default:** `"A"`

Disconnect from all non-autojoin rooms.

### `keys.rooms.action.new`

**Required:** yes  
**Type:** key binding  
**Default:** `"n"`

Connect to new room.

### `keys.scroll.center_cursor`

**Required:** yes  
**Type:** key binding  
**Default:** `"z"`

Center cursor.

### `keys.scroll.down_full`

**Required:** yes  
**Type:** key binding  
**Default:** `["ctrl+f", "pagedown"]`

Scroll down a full screen.

### `keys.scroll.down_half`

**Required:** yes  
**Type:** key binding  
**Default:** `"ctrl+d"`

Scroll down half a screen.

### `keys.scroll.down_line`

**Required:** yes  
**Type:** key binding  
**Default:** `"ctrl+e"`

Scroll down one line.

### `keys.scroll.up_full`

**Required:** yes  
**Type:** key binding  
**Default:** `["ctrl+b", "pageup"]`

Scroll up a full screen.

### `keys.scroll.up_half`

**Required:** yes  
**Type:** key binding  
**Default:** `"ctrl+u"`

Scroll up half a screen.

### `keys.scroll.up_line`

**Required:** yes  
**Type:** key binding  
**Default:** `"ctrl+y"`

Scroll up one line.

### `keys.tree.action.decrease_caesar`

**Required:** yes  
**Type:** key binding  
**Default:** `"C"`

Decrease caesar cipher rotation.

### `keys.tree.action.fold_tree`

**Required:** yes  
**Type:** key binding  
**Default:** `"space"`

Fold current message's subtree.

### `keys.tree.action.increase_caesar`

**Required:** yes  
**Type:** key binding  
**Default:** `"c"`

Increase caesar cipher rotation.

### `keys.tree.action.inspect`

**Required:** yes  
**Type:** key binding  
**Default:** `"i"`

Inspect selected element.

### `keys.tree.action.links`

**Required:** yes  
**Type:** key binding  
**Default:** `"I"`

List links found in message.

### `keys.tree.action.mark_older_seen`

**Required:** yes  
**Type:** key binding  
**Default:** `"ctrl+s"`

Mark all older messages as seen.

### `keys.tree.action.mark_visible_seen`

**Required:** yes  
**Type:** key binding  
**Default:** `"S"`

Mark all visible messages as seen.

### `keys.tree.action.new_thread`

**Required:** yes  
**Type:** key binding  
**Default:** `"t"`

Start a new thread.

### `keys.tree.action.reply`

**Required:** yes  
**Type:** key binding  
**Default:** `"r"`

Reply to message, inline if possible.

### `keys.tree.action.reply_alternate`

**Required:** yes  
**Type:** key binding  
**Default:** `"R"`

Reply opposite to normal reply.

### `keys.tree.action.toggle_seen`

**Required:** yes  
**Type:** key binding  
**Default:** `"s"`

Toggle current message's seen status.

### `keys.tree.cursor.to_above_sibling`

**Required:** yes  
**Type:** key binding  
**Default:** `["K", "ctrl+up"]`

Move to above sibling.

### `keys.tree.cursor.to_below_sibling`

**Required:** yes  
**Type:** key binding  
**Default:** `["J", "ctrl+down"]`

Move to below sibling.

### `keys.tree.cursor.to_newer_message`

**Required:** yes  
**Type:** key binding  
**Default:** `["l", "right"]`

Move to newer message.

### `keys.tree.cursor.to_newer_unseen_message`

**Required:** yes  
**Type:** key binding  
**Default:** `["L", "ctrl+right"]`

Move to newer unseen message.

### `keys.tree.cursor.to_older_message`

**Required:** yes  
**Type:** key binding  
**Default:** `["h", "left"]`

Move to older message.

### `keys.tree.cursor.to_older_unseen_message`

**Required:** yes  
**Type:** key binding  
**Default:** `["H", "ctrl+left"]`

Move to older unseen message.

### `keys.tree.cursor.to_parent`

**Required:** yes  
**Type:** key binding  
**Default:** `"p"`

Move to parent.

### `keys.tree.cursor.to_root`

**Required:** yes  
**Type:** key binding  
**Default:** `"P"`

Move to root.

### `measure_widths`

**Required:** yes  
**Type:** boolean  
**Default:** `false`

Whether to measure the width of characters as displayed by the terminal
emulator instead of guessing the width.

Enabling this makes rendering a bit slower but more accurate. The screen
might also flash when encountering new characters (or, more accurately,
graphemes).

See also the `--measure-widths` command line option.

### `offline`

**Required:** yes  
**Type:** boolean  
**Default:** `false`

Whether to start in offline mode.

In offline mode, cove won't automatically join rooms marked via the
`autojoin` option on startup. You can still join those rooms manually by
pressing `a` in the rooms list.

See also the `--offline` command line option.

### `rooms_sort_order`

**Required:** yes  
**Type:** string  
**Values:** `"alphabet"`, `"importance"`  
**Default:** `"alphabet"`

Initial sort order of rooms list.

`"alphabet"` sorts rooms in alphabetic order.

`"importance"` sorts rooms by the following criteria (in descending
order of priority):

1. connected rooms before unconnected rooms
2. rooms with unread messages before rooms without
3. alphabetic order

### `time_zone`

**Required:** no  
**Type:** string  
**Default:** `$TZ` or local system time zone

Time zone that chat timestamps should be displayed in.

This option is interpreted as a POSIX TZ string. It is described here in
further detail:
<https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap08.html>

On a normal system, the string `"localtime"` as well as any value from
the "TZ identifier" column of the following wikipedia article should be
valid TZ strings:
<https://en.wikipedia.org/wiki/List_of_tz_database_time_zones>

If the `TZ` environment variable exists, it overrides this option. If
neither exist, cove uses the system's local time zone.

**Warning:** On Windows, cove can't get the local time zone and uses UTC
instead. However, you can still specify a path to a tz data file or a
custom time zone string.
