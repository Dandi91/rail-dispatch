- Allow assigning speed restrictions in level config to every route from the signal separately
- Clearer separation between train driving modes (on time, catching up, target stop, signal creeping)

MAP EDITOR
Placing lamps on a PNG board and then defining everything in the level file is tedious. I need a way to interactively
place lamps on the board and configure them via UI. Saving the state should generate PNG + level file.
The editor should be a separate binary in the same project and perhaps reuse some visualization code from the game.
The main functions are:
- Load PNG and level file
- Draw the lamps according to the level file
- Allow interactive editing of existing lamps: changing width, position and rotation
- Allow adding new lamps
Lamps should be composed of a backlight image (a lamp node with background color) and a cover image.
Cover image is a transparent window of elliptical shape with a black border, which gives the lamp its look.
** This is the main difference with the current design **, now covers and background PNG are a single image, but this makes it hard to adjust the lamps.
There are two kinds of covers: one for block lamps and one for signal lamps (with a little post).
Covers should be rotated to match the lamp angle. 
Covers are built from left and right caps and a stretchy middle. When lamp width changes, only the middle part is stretched.
The final board composition should be as follows:
- background PNG image, loaded from the file
- backlight lamp nodes
- transparent lamp cover on top