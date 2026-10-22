Feature: Retouching

  Background:
    Given a red and blue image "rb.png" of 200 by 100 pixels
    And a "blue" image "b.png" of 40 by 80 pixels

  @SMEAR-001
  Scenario: Start and leave the smear tool
    Given I open "rb.png"
    When I select image 1
    And I press "S"
    Then the smear tool is active on image 1
    When I press "Escape"
    Then the smear tool is not active
    When I open the context menu on image 1
    And I choose "Smear"
    Then the smear tool is active on image 1
    When I press "Enter"
    Then the smear tool is not active

  @SMEAR-003
  Scenario: Brush defaults and remembered settings
    Given I open "rb.png"
    When I start smearing image 1
    Then the brush is smudge 5 pixels wide at 50% strength
    When I set the brush to blur 20 pixels wide at 80% strength
    And I press "Escape"
    And I start smearing image 1
    Then the brush is blur 20 pixels wide at 80% strength

  @SMEAR-002 @SMEAR-004
  Scenario Outline: Each kind changes the pixels under the stroke, as one step
    Given I open "rb.png"
    And the view is at 100% with the canvas origin on screen at 50, 50
    When I start smearing image 1
    And I set the brush to <kind> 20 pixels wide at 100% strength
    And I drag from 90, 50 to 110, 50
    Then the undo label is "Undo Smear"
    And the project has 2 steps
    And the rendered canvas at 101, 50 is no longer "blue"
    And the rendered canvas at 150, 50 is "blue"
    And the rendered canvas at 101, 5 is "blue"

    Examples:
      | kind   |
      | smudge |
      | blur   |
      | push   |

  @SMEAR-004
  Scenario: The stroke shows while it is drawn
    Given I open "rb.png"
    And the view is at 100% with the canvas origin on screen at 50, 50
    When I start smearing image 1
    And I set the brush to smudge 20 pixels wide at 100% strength
    And the canvas is drawn
    And I move the pointer from 90, 50 to 110, 50 without releasing
    Then the screen at canvas point 101, 50 is no longer "blue"
    And the project has 1 steps

  @SMEAR-004
  Scenario: A stroke changes only its own image
    Given a composition of "rb.png" and "b.png" side by side
    And the view is at 100% with the canvas origin on screen at 50, 50
    When I start smearing image 1
    And I set the brush to smudge 20 pixels wide at 100% strength
    And I drag from 180, 50 to 230, 50
    Then the rendered canvas at 220, 50 is "blue"

  @SMEAR-005
  Scenario: Strokes stay with the image when it moves, resizes and is saved
    Given a composition of "rb.png" with a custom canvas of 400 by 200 pixels
    And the view is at 100% with the canvas origin on screen at 50, 50
    When I start smearing image 1
    And I set the brush to smudge 20 pixels wide at 100% strength
    And I drag from 190, 100 to 210, 100
    Then the rendered canvas at 201, 100 is no longer "blue"
    When image 1 is moved to 0, 0
    Then the rendered canvas at 101, 50 is no longer "blue"
    When I select image 1
    And I resize the image to width 400 with the aspect ratio locked
    Then the rendered canvas at 203, 100 is no longer "blue"
    And the rendered canvas at 300, 100 is "blue"
    When I save the project as "p.imaginer"
    And I open "p.imaginer"
    Then the rendered canvas at 203, 100 is no longer "blue"
