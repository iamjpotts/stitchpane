Feature: Composition

  Background:
    Given a "red" image "a.png" of 100 by 50 pixels
    And a "blue" image "b.png" of 40 by 80 pixels

  @CANVAS-001
  Scenario Outline: Canvas size choices
    When I create a composition of "a.png" and "b.png" with canvas "<choice>"
    Then the canvas is <width> by <height> pixels
    And image 1 is at <x1>, <y1>
    And image 2 is at <x2>, <y2>

    Examples:
      | choice                    | width | height | x1  | y1  | x2   | y2    |
      | Side by side              | 140   | 80     | 0   | 15  | 100  | 0     |
      | Stacked                   | 100   | 130    | 0   | 0   | 30   | 50    |
      | Size of the largest image | 100   | 50     | 0   | 0   | 30   | -15   |
      | 1920 × 1080               | 1920  | 1080   | 910 | 515 | 940  | 500   |
      | 3840 × 2160               | 3840  | 2160   | 1870| 1055| 1900 | 1040  |

  @CANVAS-001
  Scenario: Paper presets set the DPI
    When I create a composition of "a.png" and "b.png" with canvas "A4 at 300 DPI"
    Then the canvas is 2480 by 3508 pixels
    And the canvas DPI is 300
    When I create a composition of "a.png" and "b.png" with canvas "US Letter at 300 DPI"
    Then the canvas is 2550 by 3300 pixels

  @CANVAS-001
  Scenario: Custom canvas size
    When I create a composition of "a.png" and "b.png" with a custom canvas of 300 by 200 pixels at 72 DPI
    Then the canvas is 300 by 200 pixels
    And the canvas DPI is 72

  @CANVAS-001
  Scenario: Opening several images defaults to side by side
    When I open "a.png" and "b.png" together
    Then the canvas is 140 by 80 pixels

  @CANVAS-002
  Scenario: Resize the canvas by numbers
    Given a composition of "a.png" and "b.png" side by side
    When I set the canvas size to 300 by 200 pixels at 96 DPI
    Then the canvas is 300 by 200 pixels
    And the canvas DPI is 96
    And the undo label is "Undo Resize canvas"

  @CANVAS-003
  Scenario: Resize the canvas with a handle
    Given a composition of "a.png" and "b.png" side by side
    And the view is at 100% with the canvas origin on screen at 50, 50
    When I drag from 140, 40 to 160, 40
    Then the canvas is 160 by 80 pixels

  @CANVAS-003
  Scenario: Growing the canvas to the left keeps images in place on screen
    Given a composition of "a.png" and "b.png" side by side
    And the view is at 100% with the canvas origin on screen at 50, 50
    And I remember where image 1 is on screen
    When I drag from 0, 40 to -20, 40
    Then the canvas is 160 by 80 pixels
    And image 1 is at 20, 15
    And image 1 is where I remembered on screen

  @CANVAS-004
  Scenario: Uncovered canvas is transparent
    Given a composition of "a.png" and "b.png" side by side
    Then the rendered canvas at 50, 5 is "transparent"

  @PLACE-001
  Scenario: Add images to a composition
    Given a composition of "a.png" and "b.png" with canvas "Size of the largest image"
    And a "green" image "c.png" of 20 by 20 pixels
    When I add "c.png"
    Then the project has 3 images
    And image 3 is at 40, 15
    And image 3 is on top

  @PLACE-001
  Scenario: Drop images onto the canvas
    Given a composition of "a.png" and "b.png" side by side
    And the view is at 100% with the canvas origin on screen at 50, 50
    And a "green" image "c.png" of 20 by 20 pixels
    When I drop "c.png" on the canvas at 70, 30
    Then image 3 is at 60, 20
    And image 3 is on top

  @PLACE-001 @UI-005
  Scenario: Add an image to a single image
    Given I open "a.png"
    When I add "b.png"
    Then the project has 2 images
    And image 2 is at 100, 0
    And the canvas is 140 by 80 pixels

  @PLACE-002
  Scenario: Drag to move
    Given a composition of "a.png" and "b.png" side by side
    And the view is at 100% with the canvas origin on screen at 50, 50
    And snapping is off
    When I drag from 50, 25 to 61, 32
    Then image 1 is at 11, 22
    And the undo label is "Undo Move image"

  @PLACE-003
  Scenario: Corner handles keep the aspect ratio
    Given a composition of "a.png" and "b.png" with a custom canvas of 400 by 400 pixels
    And the view is at 100% with the canvas origin on screen at 50, 50
    And snapping is off
    When I select image 1
    And I drag from 250, 225 to 350, 235
    Then image 1 is 200 by 100 pixels
    And image 1 is at 150, 175

  @PLACE-003
  Scenario: Shift frees the aspect ratio
    Given a composition of "a.png" and "b.png" with a custom canvas of 400 by 400 pixels
    And the view is at 100% with the canvas origin on screen at 50, 50
    And snapping is off
    When I select image 1
    And I drag from 250, 225 to 260, 245 with Shift
    Then image 1 is 110 by 70 pixels

  @PLACE-003
  Scenario: Edge handles change one dimension and never the source
    Given a composition of "a.png" and "b.png" with a custom canvas of 400 by 400 pixels
    And the view is at 100% with the canvas origin on screen at 50, 50
    And snapping is off
    When I select image 1
    And I drag from 250, 200 to 270, 200
    Then image 1 is 120 by 50 pixels
    And the undo label is "Undo Resize image"
    And "a.png" is unchanged on disk

  @PLACE-004
  Scenario: Crop an item in a composition
    Given a composition of "a.png" and "b.png" with a custom canvas of 400 by 400 pixels
    When I select image 1
    And I start cropping the image
    And I set the crop rectangle to 0, 0, 50, 50
    And I press "Enter"
    Then image 1 is 50 by 50 pixels
    And image 1 is at 150, 175
    And the canvas is 400 by 400 pixels

  @CROP-005
  Scenario: Uncrop an item cropped several steps earlier
    Given a composition of "a.png" and "b.png" with a custom canvas of 400 by 400 pixels
    And the view is at 100% with the canvas origin on screen at 50, 50
    When I select image 1
    And I start cropping the image
    And I set the crop rectangle to 0, 0, 50, 50
    And I press "Enter"
    And image 2 is moved to 10, 10
    And I select image 1
    And I start cropping the image
    Then image 1 is shown whole and on top while cropping
    And the crop rectangle is 0, 0, 50, 50
    When I drag from 200, 200 to 250, 200
    Then the crop rectangle is 0, 0, 100, 50
    When I press "Enter"
    Then image 1 is 100 by 50 pixels
    And image 1 is at 150, 175
    And image 2 is on top

  @PLACE-005
  Scenario: Change the stacking order
    Given a composition of "a.png" and "b.png" with canvas "Size of the largest image"
    When I open the context menu on image 1
    Then the menu has "Bring to front", "Bring forward", "Send backward" and "Send to back"
    When I choose "Bring to front"
    Then image 1 is on top
    When I open the context menu on image 1
    And I choose "Send backward"
    Then image 2 is on top

  @PLACE-006
  Scenario: Dragging snaps to other images and shows a guide
    Given a composition of "a.png" and "b.png" with a custom canvas of 400 by 400 pixels
    And the view is at 100% with the canvas origin on screen at 50, 50
    When I select image 2
    And I move the pointer from 200, 200 to 228, 200 without releasing
    Then a vertical guide is shown at 250
    When I release the pointer at 228, 200
    Then image 2 is at 210, 160

  @PLACE-006
  Scenario: Alt disables snapping for one drag
    Given a composition of "a.png" and "b.png" with a custom canvas of 400 by 400 pixels
    And the view is at 100% with the canvas origin on screen at 50, 50
    When I select image 2
    And I drag from 200, 200 to 228, 200 holding Alt
    Then image 2 is at 208, 160

  @PLACE-006
  Scenario: Snapping can be turned off
    Given a composition of "a.png" and "b.png" with a custom canvas of 400 by 400 pixels
    And the view is at 100% with the canvas origin on screen at 50, 50
    And snapping is off
    When I select image 2
    And I drag from 200, 200 to 228, 200
    Then image 2 is at 208, 160

  @PLACE-006 @UI-006
  Scenario: Keyboard adjustments never snap
    Given a composition of "a.png" and "b.png" with a custom canvas of 400 by 400 pixels
    When I select image 2
    And I press "Right"
    Then image 2 is at 181, 160

  @PLACE-007
  Scenario: Quarter turns rotate without resampling
    Given a patterned image "p.png" of 30 by 20 pixels
    And I open "p.png"
    When I open the context menu on image 1
    Then the menu has "Rotate 90° clockwise", "Rotate 90° counterclockwise" and "Rotate 180°"
    When I choose "Rotate 90° clockwise"
    Then the canvas is 20 by 30 pixels
    And the rendered canvas is exactly the source rotated a quarter turn clockwise

  @PLACE-008
  Scenario: Rotate with the rotation handle
    Given a composition of "a.png" and "b.png" with a custom canvas of 400 by 400 pixels
    And the view is at 100% with the canvas origin on screen at 50, 50
    And snapping is off
    When I select image 1
    And I drag from 200, 147 to 300, 200
    Then image 1 is rotated 90 degrees

  @PLACE-008
  Scenario: Shift limits the rotation handle to 15° steps
    Given a composition of "a.png" and "b.png" with a custom canvas of 400 by 400 pixels
    And the view is at 100% with the canvas origin on screen at 50, 50
    When I select image 1
    And I drag from 200, 147 to 287, 150 with Shift
    Then image 1 is rotated 60 degrees

  @PLACE-008
  Scenario: Rotate by an entered angle
    Given a composition of "a.png" and "b.png" with a custom canvas of 400 by 400 pixels
    When I rotate image 1 to 12.34 degrees
    Then image 1 is rotated 12.3 degrees
    And the undo label is "Undo Rotate image"
    And the rendered canvas at 151, 176 is "transparent"

  @PLACE-007
  Scenario Outline: Quarter turns stay pixel-aligned for any size
    Given a patterned image "q.png" of <width> by <height> pixels
    And I open "q.png"
    When I open the context menu on image 1
    And I choose "<command>"
    Then the canvas is <height> by <width> pixels
    And the rendered canvas is exactly the source rotated a quarter turn <direction>

    Examples:
      | width | height | command                      | direction        |
      | 31    | 20     | Rotate 90° clockwise         | clockwise        |
      | 31    | 20     | Rotate 90° counterclockwise  | counterclockwise |
      | 30    | 21     | Rotate 90° counterclockwise  | counterclockwise |
      | 7     | 4      | Rotate 90° clockwise         | clockwise        |

  @PLACE-002 @UNDO-004
  Scenario: A click selects without changing anything
    Given a composition of "a.png" and "b.png" with a custom canvas of 400 by 400 pixels
    And the view is at 100% with the canvas origin on screen at 50, 50
    When I drag from 200, 200 to 201, 201
    Then the project has 2 steps
    And image 2 is selected
    When I drag from 400, 200 to 401, 200
    Then the project has 2 steps
