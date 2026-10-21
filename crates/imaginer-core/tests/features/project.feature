Feature: Project file

  @PROJ-001
  Scenario: A new project is empty
    Given a new project
    Then the project has 0 sources
    And the project has 0 steps

  @PROJ-002
  Scenario: Projects are saved as YAML
    Given a new project
    And an image "photos/left.png" of 40 by 30 pixels added to the project
    When I save the project as "trip.imaginer"
    Then "trip.imaginer" is a YAML document

  @PROJ-003
  Scenario: A newer format version is refused
    Given a project file "future.imaginer" containing:
      """
      version: 2
      canvas: { size: fit }
      sources: []
      steps: []
      """
    When I open "future.imaginer"
    Then opening fails with a message containing "version 2"
    And the message contains "version 1"
    And "future.imaginer" is unchanged

  @PROJ-004
  Scenario: Source paths are relative to the project file
    Given a new project
    And an image "photos/left.png" of 40 by 30 pixels added to the project
    And an image "shared/right.png" of 40 by 30 pixels added to the project
    When I save the project as "projects/trip.imaginer"
    Then "projects/trip.imaginer" lists the source path "../photos/left.png"
    And "projects/trip.imaginer" lists the source path "../shared/right.png"

  @PROJ-004
  Scenario: Saving to a new location rewrites source paths
    Given a new project
    And an image "photos/left.png" of 40 by 30 pixels added to the project
    When I save the project as "trip.imaginer"
    And I save the project as "deep/er/trip.imaginer"
    Then "trip.imaginer" lists the source path "photos/left.png"
    And "deep/er/trip.imaginer" lists the source path "../../photos/left.png"

  @PROJ-004 @PROJ-006
  Scenario: Reopening resolves relative paths and replays steps
    Given a new project
    And an image "photos/left.png" of 40 by 30 pixels added to the project
    And the image is moved to 15, 25
    When I save the project as "projects/trip.imaginer"
    And I open "projects/trip.imaginer"
    Then the project's source 1 is the file "photos/left.png"
    And image 1 is at 15, 25

  @PROJ-005
  Scenario: A project with a missing source still opens
    Given a new project
    And an image "photos/left.png" of 40 by 30 pixels added to the project
    When I save the project as "trip.imaginer"
    And the file "photos/left.png" is deleted
    And I open "trip.imaginer"
    Then the project has 1 sources
    And source 1 is missing

  @PROJ-006
  Scenario: Steps are stored one per action, in order
    Given a new project
    And an image "photos/left.png" of 40 by 30 pixels added to the project
    And the image is moved to 1, 0
    And the image is moved to 2, 0
    And the image is moved to 3, 0
    When I save the project as "trip.imaginer"
    Then "trip.imaginer" has 4 steps

  @PROJ-007
  Scenario: Unknown fields are refused
    Given a project file "odd.imaginer" containing:
      """
      version: 1
      canvas: { size: fit }
      sources: []
      steps: []
      colour: blue
      """
    When I open "odd.imaginer"
    Then opening fails with a message containing "colour"

  @PROJ-007
  Scenario: Unknown step types are refused
    Given a project file "odd.imaginer" containing:
      """
      version: 1
      canvas: { size: fit }
      sources: []
      steps:
        - sharpen_item: { item: 1 }
      """
    When I open "odd.imaginer"
    Then opening fails with a message containing "sharpen_item"

  @PROJ-008 @UNDO-005
  Scenario: Undo and redo history survive saving
    Given a new project
    And an image "photos/left.png" of 40 by 30 pixels added to the project
    And the image is moved to 10, 0
    And the image is moved to 20, 0
    When I undo
    And I save the project as "trip.imaginer"
    And I open "trip.imaginer"
    Then image 1 is at 10, 0
    And the redo label is "Redo Move image"
    When I redo
    Then image 1 is at 20, 0
    When I undo
    And I undo
    Then the undo label is "Undo Add image"

  @PROJ-005
  Scenario: Missing sources are labeled and can be located
    Given a new project
    And an image "photos/left.png" of 40 by 30 pixels added to the project
    And a "blue" image "elsewhere/left.png" of 40 by 30 pixels
    When I save the project as "trip.imaginer"
    And the file "photos/left.png" is deleted
    And I open "trip.imaginer"
    Then image 1 is labeled "Missing: left.png"
    And the rendered canvas at 20, 15 is gray
    When I locate source 1 at "elsewhere/left.png"
    Then no sources are missing
    And the rendered canvas at 20, 15 is "blue"
    And the undo label is "Undo Locate source"
