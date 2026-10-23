Feature: Editing

  Background:
    Given a "red" image "a.png" of 100 by 50 pixels
    And I open "a.png"

  @UNDO-001
  Scenario: Ctrl+Z undoes
    Given image 1 is moved to 5, 5
    When I press "Ctrl+Z"
    Then image 1 is at 0, 0

  @UNDO-002
  Scenario Outline: Redo shortcuts
    Given image 1 is moved to 5, 5
    When I press "Ctrl+Z"
    And I press "<shortcut>"
    Then image 1 is at 5, 5

    Examples:
      | shortcut     |
      | Ctrl+Shift+Z |
      | Ctrl+Y       |

  @UNDO-003
  Scenario: Undo and redo are disabled when there is nothing to do
    Then undo is available
    And redo is not available
    When I press "Ctrl+Z"
    Then undo is not available
    And redo is available
    And the redo label is "Redo Add image"

  @UNDO-004
  Scenario: A drag is one step
    Given the view is at 100% with the canvas origin on screen at 50, 50
    And snapping is off
    When I drag from 50, 25 through 55, 25 and 60, 30 to 70, 35
    Then the project has 2 steps
    And image 1 is at 20, 10

  @UNDO-004 @UI-006
  Scenario: A run of key presses is one step
    When I select image 1
    And I press "Right" 3 times
    Then image 1 is at 3, 0
    And the project has 1 steps
    When I press "Escape"
    Then the project has 2 steps
    When I press "Ctrl+Z"
    Then image 1 is at 0, 0

  @UNDO-004
  Scenario: A committed numeric value is one step
    When I resize the image to 50 by 25 pixels
    Then the project has 2 steps
