<#-- General -->
loading = Loading
create = Create
cancel = Cancel
delete = Delete
rename = Rename
move = Move
move-to = Move To
open-file-manager = Reveal in File Manager
name = Name
copied-to-clipboard = Copied to Clipboard
file-saved = File Saved!
untitled = Untitled

<#-- About Page -->
repository = Repository
support = Support
donations = Donate

<#-- Main App -->
editor = Editor
preview = Preview
file-name = File Name
folder-name = Folder Name
delete-node = Delete File/Folder
delete-confirmation = Are you sure you want to delete this file/folder and it's contents?
save-changes-closing = Save changes before closing?
save-warning = You have unsaved changes. If you continue without saving, these changes will be lost.
discard-changes = Discard Changes
file-changed-externally = The file has been changed externally
reload = Reload File
keep-my-version = Keep Current Version
modified-text = was modified by another program.
loading-image = Loading image... (if this persists the app may not have permissions to read the file)
words = { $count ->
    [one] 1 word
   *[other] { $count } words
}
characters = { $count ->
    [one] 1 character
   *[other] { $count } characters
}
reading-time = { $minutes ->
    [0] < 1 min read
    [one] 1 min read
   *[other] { $minutes } min read
}

<#-- Appearance -->
appearance = Appearance
theme = Theme
match-desktop = Match desktop
dark = Dark
light = Light
dark-highlighter = Dark Highlighter Theme
light-highlighter = Light Highlighter Theme
highlighter-theme-colors = Use highlighter theme colors
highlighter-theme-colors-description = Paint the editor and code blocks with the background and text colors of the highlighter theme
custom-themes = Custom highlighter themes
custom-themes-info = Import your own highlighter themes: Cedilla RON themes, COSMIC Terminal color schemes (.ron) and TextMate/Sublime themes (.tmTheme). Files dropped in the themes folder are loaded on startup or after pressing reload.
import-theme = Import theme…
open-themes-folder = Themes folder
reload-themes = Reload themes
theme-imported = Imported theme: { $name }
theme-import-failed = Could not import theme: { $error }

<#-- Window -->
window = Window
frosted-glass = Frosted glass
frosted-glass-description = Blur the desktop behind the window
frosted-glass-info = Frosted glass needs a compositor with blur support (COSMIC). "Follow desktop" uses the "Frosted windows" setting of the COSMIC appearance settings. Lower the window opacity to see the effect.
frosted-follow-system = Follow desktop
frosted-enabled = Enabled
frosted-disabled = Disabled
window-opacity = Window opacity

<#-- Settings Context Page -->
general = General
open-vault = Use another folder as vault
open-vault-info = Pick any existing folder and use it as the vault without moving anything. Useful to work on the notes of another Cedilla installation, e.g. the Flatpak one keeps its vault in ~/.var/app/dev.mariinkys.Cedilla/data/dev.mariinkys.Cedilla/vault
help-bar = Editor Help Bar
status-bar = Status Bar
move-vault = Move Vault
current-location = Current Location:
show = Show
hide = Hide
last-file = Open the last file when the app starts
yes = Yes
no = No
scrollbar-sync = Sync Preview and Editor scrollbars
autosave = Autosave
autosave-description = Save open files automatically after a pause in typing
autosave-off = Off
autosave-seconds = { $seconds }s
pdf-exporting = PDF Exporting
gotenberg-url = Set the Gotenberg server URL
apply = Apply
more-info = More Info
text-size = Text Size
vault-default-location = Default (App Data Directory)
flatpak-permissions = By default, the Flatpak version only has permissions for the Documents, Downloads, and Pictures folders. Moving the vault to any other location will make the app stop working correctly. If you want to use other directories, please tweak the permissions with tools like Flatseal.
flatpak-permissions-note = Note, this also affects image loading. Local images will only load if they are in a folder with permissions.
selected-font = Selected Font
default-font = Default Font
font-selection-info = Some fonts will not work, and the app will automatically fall back to the default font (without changing the selected font in the settings). That is not a bug; it is the intended behavior.

<#-- Application MenuBar -->
file = File
new-vault-file = New Vault File
new-folder = New Folder
open-file = Open File
save-file = Save File
new-file = New File
export = Export
export-pdf = Export as PDF…
export-html = Export as HTML…
copy-html = Copy as HTML
html-exported = Exported as HTML
pdf-exported = Exported as PDF

edit = Edit
undo = Undo
redo = Redo
cut = Cut
copy = Copy
paste = Paste
select-all = Select All
paste-image = Paste Image
no-image-clipboard = No image found in clipboard
image-pasted = Image saved: { $name }
insert-datetime = Insert Date & Time

format = Format

view = View
view-editor = Editor only
view-split = Editor and Preview
view-preview = Preview only
toggle-preview = Toggle Preview
zen-mode = Zen Mode
outline = Outline
no-headings = This document has no headings yet
about = About
settings = Settings

<#-- Helper HeaderBar -->
bold = Bold
italic = Italic
link = Hyperlink
code = Code
image = Image
attachment = Attachment
bullet-list = Bulleted List
numbered-list = Numbered List
checkbox = Checkbox
horizontal-rule = Horizontal Rule
strikethrough = Strikethrough
quote = Quote
table = Table

<#-- Search -->
search = Search
no-results = No Results
search-vault = Search in Vault
search-vault-placeholder = Search every note…
search-vault-hint = Type to search across all the notes of the vault
searching = Searching…
vault-results = { $count ->
    [0] No results
    [one] 1 result
   *[other] { $count } results
}
