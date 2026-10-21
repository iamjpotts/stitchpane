Feature: Export

  Background:
    Given a "red" image "a.png" of 100 by 50 pixels

  @EXPORT-001
  Scenario: Export renders to a new file
    Given I open "a.png"
    When I export to "out.png"
    Then "out.png" is a PNG image of 100 by 50 pixels

  @EXPORT-001
  Scenario: The format follows the file extension
    Given I open "a.png"
    When I prepare to export to "OUT.JPG"
    Then the export format is "JPEG"

  @EXPORT-002
  Scenario Outline: DPI metadata
    Given I open "a.png"
    And I set the DPI to 300
    When I export to "out.<ext>"
    Then "out.<ext>" records 300 DPI

    Examples:
      | ext |
      | png |
      | jpg |
      | tif |

  @EXPORT-003
  Scenario: Exporting over a source needs confirmation
    Given I open "a.png"
    Then exporting to "a.png" would overwrite a source
    And exporting to "b.png" would not overwrite a source

  @EXPORT-004
  Scenario Outline: Transparency
    Given a "blue" image "b.png" of 40 by 80 pixels
    And a composition of "a.png" and "b.png" side by side
    When I export to "out.<ext>"
    Then "out.<ext>" at 50, 5 is "<color>"

    Examples:
      | ext | color       |
      | jpg | white       |
      | png | transparent |
      | bmp | white       |

  @EXPORT-001
  Scenario: Nothing to export
    When I export to "out.png"
    Then exporting fails with a message containing "nothing to export"
