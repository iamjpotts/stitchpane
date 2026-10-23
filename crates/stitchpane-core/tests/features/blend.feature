Feature: Blend transitions

  Background:
    Given a "red" image "left.png" of 120 by 100 pixels
    And a "blue" image "right.png" of 120 by 100 pixels
    And a composition of "left.png" and "right.png" side by side
    And image 2 is moved to 80, 0
    And the view is at 100% with the canvas origin on screen at 50, 50

  @BLEND-006
  Scenario: Three kinds of transition are offered
    When I open the context menu on image 1
    Then the menu has "Horizontal gradient seam with right.png", "Vertical gradient seam with right.png" and "Crossfade with right.png"
    And the menu has "Feather edges…"

  @BLEND-001
  Scenario: Create a gradient seam
    When I open the context menu on image 1
    And I choose "Horizontal gradient seam with right.png"
    Then there is 1 transition
    And the seam is at 100 with a band 20 wide
    And the rendered canvas at 85, 50 is "red"
    And the rendered canvas at 115, 50 is "blue"
    And the rendered canvas at 100, 50 is a mix of red and blue

  @BLEND-001
  Scenario: Create a vertical gradient seam
    When I open the context menu on image 1
    And I choose "Vertical gradient seam with right.png"
    Then the seam is vertical at 50 with a band 50 wide

  @BLEND-001
  Scenario: Images that do not overlap get no transition options
    Given image 2 is moved to 130, 0
    When I open the context menu on image 1
    Then the menu does not have "Crossfade with right.png"

  @BLEND-002
  Scenario: Drag the seam and the band edges
    Given a horizontal gradient seam between image 1 and image 2
    When I select image 1
    And I drag from 100, 30 to 110, 30
    Then the seam is at 110 with a band 20 wide
    When I drag from 120, 30 to 130, 30
    Then the seam is at 110 with a band 40 wide

  @BLEND-003
  Scenario: Transition properties
    Given a horizontal gradient seam between image 1 and image 2
    When I select image 1
    And I open the context menu at 100, 30
    Then the menu has "Properties…", "Change to crossfade" and "Remove"
    When I set the seam to vertical, 30 wide, with a smooth curve
    Then the seam is vertical at 50 with a band 30 wide
    And the seam curve is smooth
    When I open the context menu on image 1
    And I choose "Gradient seam with right.png…"
    Then the UI is asked for the transition properties dialog
    When I open the context menu on image 1
    And I choose "Remove gradient seam with right.png"
    Then there are 0 transitions

  @BLEND-003
  Scenario: Switch between gradient seam and crossfade
    Given a horizontal gradient seam between image 1 and image 2
    When I select image 1
    And I open the context menu at 100, 30
    And I choose "Change to crossfade"
    Then the transition is a crossfade at 50%

  @BLEND-007
  Scenario: Feather edges with the default width
    Given I open "left.png"
    When I open the context menu on image 1
    And I choose "Feather edges…"
    Then the UI is asked for the feather dialog
    And the default feather width is 10
    When I feather the left edge by 10 pixels
    Then the rendered canvas at 0, 50 is nearly transparent
    And the rendered canvas at 60, 50 is "red"
    And the undo label is "Undo Feather edges"

  @BLEND-007 @UI-006
  Scenario: Drag and nudge a feathered edge
    Given I open "left.png"
    And the view is at 100% with the canvas origin on screen at 50, 50
    When I feather the left edge by 10 pixels
    And I select image 1
    And I drag from 10, 50 to 20, 50
    Then image 1's feather width is 20
    When I press "Right"
    Then image 1's feather width is 21

  @BLEND-007
  Scenario: Feathered edges move and resize with the item but stay put when it rotates
    Given a composition of "left.png" with a custom canvas of 400 by 400 pixels
    When I feather the top edge by 10 pixels
    Then the rendered canvas alpha at 200, 155 is about 140
    When image 1 is moved to 140, 160
    Then the rendered canvas alpha at 200, 165 is about 140
    When I resize image 1 to 240 by 200 pixels
    Then image 1's feather width is 20
    When I resize image 1 to 120 by 100 pixels
    And I rotate image 1 to 90 degrees
    Then the rendered canvas alpha at 200, 165 is about 140

  @BLEND-008
  Scenario: Crossfade ratio
    When I open the context menu on image 1
    And I choose "Crossfade with right.png"
    Then the transition is a crossfade at 50%
    And the rendered canvas at 100, 50 is roughly 128, 0, 128
    When I set the crossfade ratio to 25%
    Then the rendered canvas at 100, 50 is roughly 191, 0, 64

  @BLEND-004
  Scenario: Three overlapping items composite bottom to top
    Given a "green" image "mid.png" of 40 by 100 pixels
    And a crossfade between image 1 and image 2
    When I add "mid.png"
    And I add a crossfade between image 2 and image 3
    Then the rendered canvas at 110, 50 is roughly 64, 128, 64

  @BLEND-005
  Scenario: Seams follow their items
    Given a horizontal gradient seam between image 1 and image 2
    When image 2 is moved to 90, 0
    Then the seam is at 105 with a band 20 wide
    When image 2 is moved to 130, 0
    Then there is 1 transition
    And the transition has no effect
    And the rendered canvas at 135, 50 is "blue"

  @BLEND-005
  Scenario: Seams stay put when an item rotates
    Given a horizontal gradient seam between image 1 and image 2
    When I rotate image 2 to 90 degrees
    Then the seam is at 100 with a band 20 wide
