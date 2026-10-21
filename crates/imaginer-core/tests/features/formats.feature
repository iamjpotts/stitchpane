Feature: Image formats

  @CONV-001
  Scenario Outline: Readable formats
    Given a "blue" image "picture.<ext>" of 30 by 20 pixels
    When I open "picture.<ext>"
    Then the canvas is 30 by 20 pixels
    And the rendered canvas at 15, 10 is roughly "blue"

    Examples:
      | ext  |
      | png  |
      | jpg  |
      | webp |
      | tif  |
      | bmp  |
      | gif  |
      | heic |
      | avif |

  @CONV-002
  Scenario Outline: Writable formats
    Given a "green" image "picture.png" of 30 by 20 pixels
    And I open "picture.png"
    When I export to "out.<ext>"
    Then "out.<ext>" is a <format> image of 30 by 20 pixels
    And "out.<ext>" at 15, 10 is roughly "green"

    Examples:
      | ext  | format |
      | png  | PNG    |
      | jpg  | JPEG   |
      | webp | WebP   |
      | tif  | TIFF   |
      | bmp  | BMP    |
      | heic | HEIF   |
      | avif | AVIF   |

  @CONV-002
  Scenario: GIF is not offered for export
    Then GIF is not a writable format

  @CONV-003
  Scenario: Lossy formats have a quality setting
    Given a noisy image "noise.png" of 64 by 64 pixels
    And I open "noise.png"
    When I prepare to export to "out.jpg"
    Then the export quality is 90
    And the export format is lossy
    When I export to "low.jpg" at quality 10
    And I export to "high.jpg" at quality 100
    Then "high.jpg" is larger than "low.jpg"
