Feature: Project

  @PROJ-001
  Scenario: A new project is empty
    Given a new project
    Then the project has 0 sources
