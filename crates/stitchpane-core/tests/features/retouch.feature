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
    When I save the project as "p.stitchpane"
    And I open "p.stitchpane"
    Then the rendered canvas at 203, 100 is no longer "blue"

  @LIGHT-001
  Scenario: Start, apply and cancel the light tool
    Given a "gray" image "g.png" of 100 by 60 pixels
    And I open "g.png"
    And the view is at 100% with the canvas origin on screen at 50, 50
    When I select image 1
    And I press "L"
    Then the light tool is active on image 1
    When I set the light exposure to 1
    Then the screen at canvas point 50, 30 is no longer "gray"
    And the project has 1 steps
    When I press "Escape"
    Then the light tool is not active
    And the rendered canvas at 50, 30 is "gray"
    When I open the context menu on image 1
    And I choose "Adjust light"
    And I set the light exposure to 1
    And I press "Enter"
    Then the light tool is not active
    And the undo label is "Undo Adjust light"
    And the project has 2 steps
    And the rendered canvas at 50, 30 is roughly 176, 176, 176

  @LIGHT-001
  Scenario: Reset sets every setting back to neutral
    Given a "gray" image "g.png" of 100 by 60 pixels
    And I open "g.png"
    When I start adjusting the light of image 1
    And I set the light exposure to 2
    And I set the light shadows to 50
    And I set the light midtones to 1.5
    And I reset the light
    Then the light tool is active on image 1
    And the light tool shows exposure 0
    And the light tool shows shadows 0
    And the light tool shows midtones 1
    When I apply the light
    Then the project has 1 steps

  @LIGHT-002
  Scenario Outline: Exposure doubles or halves the light per stop
    Given a "gray" image "g.png" of 100 by 60 pixels
    And I open "g.png"
    When I start adjusting the light of image 1
    And I set the light exposure to <stops>
    And I apply the light
    Then the rendered canvas at 50, 30 is roughly <value>, <value>, <value>

    Examples:
      | stops | value |
      | 1     | 176   |
      | -1    | 92    |

  @LIGHT-002 @LIGHT-004
  Scenario: Settings out of range are refused
    Given a "gray" image "g.png" of 100 by 60 pixels
    And I open "g.png"
    When I start adjusting the light of image 1
    And I set the light exposure to 6
    Then the light setting is refused with a message containing "exposure"
    And the light tool shows exposure 0
    When I set the light black to 200
    And I set the light white to 100
    Then the light setting is refused with a message containing "black point below the white point"
    And the light tool shows black 200

  @LIGHT-003
  Scenario: Shadows lighten the shade and leave the sunlight
    Given a shaded image "s.png" of 200 by 100 pixels
    And I open "s.png"
    And I remember the rendered canvas
    When I start adjusting the light of image 1
    And I set the light shadows to 80
    And I apply the light
    Then the rendered canvas at 20, 50 is lighter than remembered
    And the rendered canvas at 97, 50 is lighter than remembered
    And the rendered canvas at 150, 50 is as remembered

  @LIGHT-003
  Scenario: Highlights darken the sunlight and leave the shade
    Given a shaded image "s.png" of 200 by 100 pixels
    And I open "s.png"
    And I remember the rendered canvas
    When I start adjusting the light of image 1
    And I set the light highlights to 80
    And I apply the light
    Then the rendered canvas at 180, 50 is darker than remembered
    And the rendered canvas at 103, 50 is darker than remembered
    And the rendered canvas at 50, 50 is as remembered

  @LIGHT-004
  Scenario: Levels spread the values between the black and white points
    Given a "gray" image "g.png" of 100 by 60 pixels
    And I open "g.png"
    When I start adjusting the light of image 1
    And I set the light black to 28
    And I set the light white to 228
    And I set the light midtones to 2
    And I apply the light
    Then the rendered canvas at 50, 30 is roughly 180, 180, 180

  @LIGHT-006
  Scenario: Adjusting again replaces the settings and they stay with the image
    Given a "gray" image "g.png" of 100 by 60 pixels
    And I open "g.png"
    When I start adjusting the light of image 1
    And I set the light exposure to 1
    And I apply the light
    And I remember the rendered canvas
    And I start adjusting the light of image 1
    Then the light tool shows exposure 1
    When I apply the light
    Then the project has 2 steps
    And the rendered canvas at 50, 30 is as remembered
    When I start adjusting the light of image 1
    And I set the light exposure to 2
    And I apply the light
    Then the rendered canvas at 50, 30 is lighter than remembered
    When I undo
    Then the rendered canvas at 50, 30 is as remembered
    When image 1 is moved to 10, 10
    And I select image 1
    And I resize the image to width 200 with the aspect ratio locked
    Then the rendered canvas at 100, 60 is roughly 176, 176, 176
    When I save the project as "p.stitchpane"
    And I open "p.stitchpane"
    Then the rendered canvas at 100, 60 is roughly 176, 176, 176

  @DODGE-001
  Scenario: Start and leave the dodge and burn tool
    Given I open "rb.png"
    When I select image 1
    And I press "D"
    Then the dodge and burn tool is active on image 1
    When I press "Escape"
    Then the dodge and burn tool is not active
    When I open the context menu on image 1
    And I choose "Dodge and burn"
    Then the dodge and burn tool is active on image 1
    When I press "Enter"
    Then the dodge and burn tool is not active

  @DODGE-003
  Scenario: Dodge and burn brush defaults and remembered settings
    Given I open "rb.png"
    When I start dodging image 1
    Then the dodge brush is dodge midtones 5 pixels wide at 50% strength
    When I set the dodge brush to burn shadows 20 pixels wide at 80% strength
    And I press "Escape"
    And I start smearing image 1
    Then the brush is smudge 5 pixels wide at 50% strength
    When I start dodging image 1
    Then the dodge brush is burn shadows 20 pixels wide at 80% strength

  @DODGE-002 @DODGE-004
  Scenario Outline: Dodge lightens and burn darkens under the stroke, as one step
    Given a "gray" image "g.png" of 200 by 100 pixels
    And I open "g.png"
    And I remember the rendered canvas
    And the view is at 100% with the canvas origin on screen at 50, 50
    When I start dodging image 1
    And I set the dodge brush to <kind> midtones 20 pixels wide at 100% strength
    And I drag from 90, 50 to 110, 50
    Then the undo label is "Undo <label>"
    And the project has 2 steps
    And the rendered canvas at 100, 50 is <change> than remembered
    And the rendered canvas at 150, 50 is as remembered
    And the rendered canvas at 100, 20 is as remembered

    Examples:
      | kind  | label | change  |
      | dodge | Dodge | lighter |
      | burn  | Burn  | darker  |

  @DODGE-002
  Scenario: The range limits dodging to the shadows
    Given a shaded image "s.png" of 200 by 100 pixels
    And I open "s.png"
    And I remember the rendered canvas
    And the view is at 100% with the canvas origin on screen at 50, 50
    When I start dodging image 1
    And I set the dodge brush to dodge shadows 20 pixels wide at 100% strength
    And I drag from 10, 50 to 190, 50
    Then the rendered canvas at 50, 50 is lighter than remembered
    And the rendered canvas at 150, 50 is as remembered

  @DODGE-004
  Scenario: Dodging shows while drawn, changes only its own image, and stays with it
    Given a "gray" image "g.png" of 200 by 100 pixels
    And a "gray" image "g2.png" of 200 by 100 pixels
    And a composition of "g.png" and "g2.png" side by side
    And I remember the rendered canvas
    And the view is at 100% with the canvas origin on screen at 50, 50
    When I start dodging image 1
    And I set the dodge brush to dodge midtones 20 pixels wide at 100% strength
    And the canvas is drawn
    And I move the pointer from 170, 50 to 230, 50 without releasing
    Then the screen at canvas point 190, 50 is no longer "gray"
    And the project has 2 steps
    When I release the pointer at 230, 50
    Then the rendered canvas at 190, 50 is lighter than remembered
    And the rendered canvas at 210, 50 is as remembered
    When I save the project as "d.stitchpane"
    And I open "d.stitchpane"
    Then the rendered canvas at 190, 50 is lighter than remembered
