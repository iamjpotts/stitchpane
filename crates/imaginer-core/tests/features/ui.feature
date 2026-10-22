Feature: User interface behavior shared by both UIs

  Background:
    Given a "red" image "a.png" of 100 by 50 pixels

  @UI-001
  Scenario: Open an image
    When I open "a.png"
    Then the project has 1 images
    And the canvas follows the image

  @UI-001
  Scenario: Open a project
    Given I open "a.png"
    And image 1 is moved to 5, 5
    And I save the project as "p.imaginer"
    When I open "a.png"
    And I open "p.imaginer"
    Then image 1 is at 5, 5
    And the title is "p.imaginer — Imaginer"

  @UI-002
  Scenario: Unsaved changes are detected
    Given I open "a.png"
    Then there are no unsaved changes
    When image 1 is moved to 5, 5
    Then there are unsaved changes
    And the title is "*Untitled — Imaginer"
    When I save the project as "p.imaginer"
    Then there are no unsaved changes

  @UI-003
  Scenario: Zoom with the keyboard
    Given I open "a.png"
    And the view is at 100% with the canvas origin on screen at 50, 50
    When I press "Ctrl+Plus"
    Then the zoom is 125%
    When I press "Ctrl+Minus"
    Then the zoom is 100%
    When I press "Ctrl+0"
    Then the canvas fits the view
    When I press "Ctrl+1"
    Then the zoom is 100%

  @UI-003
  Scenario: Ctrl+scroll zooms about the pointer
    Given I open "a.png"
    And the view is at 100% with the canvas origin on screen at 50, 50
    When I scroll up with Ctrl at 100, 40
    Then the zoom is 125%
    And canvas point 100, 40 is still under the pointer

  @UI-004
  Scenario: Item context menu
    Given I open "a.png"
    When I open the context menu on image 1
    Then the menu has "Crop", "Resize…" and "Print size…"
    And the menu has "Rotate…" and "Bring to front"

  @UI-004
  Scenario: Canvas context menu
    Given I open "a.png"
    And the view is at 100% with the canvas origin on screen at 50, 50
    When I open the context menu at 300, 300
    Then the menu has "Add images…", "Canvas size…" and "Set DPI…"

  @UI-005
  Scenario: A single image uses the composition tools
    Given a "blue" image "b.png" of 100 by 50 pixels
    And I open "a.png"
    When I add "b.png"
    And image 2 is moved to 50, 0
    And I open the context menu on image 1
    Then the menu has "Crossfade with b.png"

  @UI-006
  Scenario: Arrow keys move the selected item
    Given I open "a.png"
    When I select image 1
    And I press "Right"
    And I press "Shift+Down"
    Then image 1 is at 1, 10

  @UI-006
  Scenario: Ctrl+arrow keys resize the selected item
    Given I open "a.png"
    When I select image 1
    And I press "Ctrl+Right"
    And I press "Ctrl+Shift+Down"
    Then image 1 is 101 by 60 pixels

  @UI-006
  Scenario: Brackets rotate the selected item
    Given I open "a.png"
    When I select image 1
    And I press "]"
    And I press "Shift+]"
    Then image 1 is rotated 1.1 degrees
    When I press "["
    Then image 1 is rotated 1 degrees

  @UI-006
  Scenario: Arrow keys move and resize the crop rectangle
    Given I open "a.png"
    When I start cropping the image
    And I set the crop rectangle to 10, 10, 50, 30
    And I press "Right"
    And I press "Ctrl+Shift+Down"
    Then the crop rectangle is 11, 10, 50, 40

  @UI-006
  Scenario: Arrow keys move a selected seam
    Given a "blue" image "b.png" of 100 by 50 pixels
    And a composition of "a.png" and "b.png" side by side
    And image 2 is moved to 60, 0
    And a horizontal gradient seam between image 1 and image 2
    When I select the transition
    And I press "Shift+Left"
    Then the seam is at 70 with a band 20 wide

  @UI-007
  Scenario: File dialogs start in the folder of the last file
    Given a "blue" image "photos/b.png" of 40 by 30 pixels
    Then file dialogs start in the default folder
    When I open "photos/b.png"
    Then file dialogs start in the folder of "photos/b.png"
    When I save the project as "projects/p.imaginer"
    Then file dialogs start in the folder of "projects/p.imaginer"
    When I add "a.png"
    Then file dialogs start in the folder of "a.png"
    When I export to "photos/out.png"
    Then file dialogs start in the folder of "photos/out.png"
    When I pick "elsewhere/c.png" in a file dialog
    Then file dialogs start in the folder of "elsewhere/c.png"
    When I open "projects/p.imaginer"
    Then file dialogs start in the folder of "projects/p.imaginer"

  @UI-009
  Scenario: Resizing draws without waiting for the image to be resampled
    Given the screen is resampled in the background
    And I open "a.png"
    And the view is at 100% with the canvas origin on screen at 50, 50
    When I select image 1
    And the canvas is drawn
    And I move the pointer from 100, 50 to 200, 100 without releasing
    Then the frame is drawn while images are still being resampled
    When I release the pointer at 200, 100
    And the background resampling finishes
    Then the frame matches one drawn without background resampling
