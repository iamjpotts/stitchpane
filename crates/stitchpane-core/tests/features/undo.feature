Feature: Undo and redo

  @UNDO-001 @UNDO-002
  Scenario: Undo and redo a step
    Given a new project
    And an image "a.png" of 40 by 30 pixels added to the project
    And the image is moved to 5, 5
    When I undo
    Then image 1 is at 0, 0
    When I redo
    Then image 1 is at 5, 5

  @UNDO-002
  Scenario: A new step discards the redo history
    Given a new project
    And an image "a.png" of 40 by 30 pixels added to the project
    And the image is moved to 5, 5
    When I undo
    And the image is moved to 7, 7
    Then there is nothing to redo

  @UNDO-003
  Scenario: Undo and redo name the step
    Given a new project
    Then there is nothing to undo
    And there is nothing to redo
    Given an image "a.png" of 40 by 30 pixels added to the project
    And the image is moved to 5, 5
    Then the undo label is "Undo Move image"
    When I undo
    Then the undo label is "Undo Add image"
    And the redo label is "Redo Move image"
