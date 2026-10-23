Feature: Single image

  Background:
    Given a "red" image "photo.png" of 200 by 100 pixels
    And I open "photo.png"
    And the view is at 100% with the canvas origin on screen at 50, 50

  @CROP-001
  Scenario: Crop handles resize and move the crop rectangle
    When I start cropping the image
    Then the crop rectangle is 0, 0, 200, 100
    When I drag from 0, 0 to 20, 10
    Then the crop rectangle is 20, 10, 180, 90
    When I drag from 100, 50 to 90, 45
    Then the crop rectangle is 10, 5, 180, 90

  @CROP-002
  Scenario: Crop by numbers
    When I start cropping the image
    And I set the crop rectangle to 10, 20, 50, 40
    Then the crop rectangle is 10, 20, 50, 40
    When I drag from 60, 60 to 70, 60
    Then the crop rectangle is 10, 20, 60, 40

  @CROP-003
  Scenario Outline: Aspect ratio choices
    When I start cropping the image
    And I choose the crop aspect ratio "<ratio>"
    Then the crop rectangle has an aspect ratio of <value>

    Examples:
      | ratio    | value |
      | Free     | 2     |
      | Original | 2     |
      | 1:1      | 1     |
      | 4:3      | 1.333 |
      | 3:2      | 1.5   |
      | 16:9     | 1.778 |
      | 3:4      | 0.75  |

  @CROP-003
  Scenario: Handles keep the chosen ratio
    When I start cropping the image
    And I choose the crop aspect ratio "1:1"
    Then the crop rectangle is 50, 0, 100, 100
    When I drag from 150, 100 to 130, 90
    Then the crop rectangle has an aspect ratio of 1

  @CROP-004
  Scenario: Applying records a crop step
    When I start cropping the image
    And I set the crop rectangle to 10, 20, 50, 40
    And I press "Enter"
    Then the crop tool is not active
    And the undo label is "Undo Crop"
    And the canvas is 50 by 40 pixels

  @CROP-004
  Scenario: Cancelling records nothing
    When I start cropping the image
    And I set the crop rectangle to 10, 20, 50, 40
    And I press "Escape"
    Then the crop tool is not active
    And the project has 1 steps
    And the canvas is 200 by 100 pixels

  @CROP-005
  Scenario: Enlarge a crop made several steps earlier
    When I start cropping the image
    And I set the crop rectangle to 10, 20, 50, 40
    And I press "Enter"
    And I set the DPI to 300
    And I start cropping the image
    Then the crop rectangle is 10, 20, 50, 40
    When I set the crop rectangle to 0, 0, 200, 100
    And I press "Enter"
    Then the canvas is 200 by 100 pixels

  @RES-001
  Scenario: Resample with the aspect ratio locked
    When I resize the image to width 100 with the aspect ratio locked
    Then the image is 100 by 50 pixels
    And the canvas is 100 by 50 pixels
    And the undo label is "Undo Resize image"

  @RES-001
  Scenario: Resample with the aspect ratio unlocked
    When I resize the image to 100 by 80 pixels
    Then the image is 100 by 80 pixels

  @RES-002
  Scenario Outline: Resampling filters
    Then the image's resampling filter is "Lanczos3"
    When I resize the image to 50 by 25 pixels using "<filter>"
    Then the image's resampling filter is "<filter>"
    And the rendered canvas at 25, 12 is "red"

    Examples:
      | filter   |
      | Nearest  |
      | Bilinear |
      | Lanczos3 |

  @RES-003
  Scenario: Set DPI without resampling
    When I set the DPI to 300
    Then the canvas DPI is 300
    And the image is 200 by 100 pixels
    And the undo label is "Undo Set DPI"

  @RES-004
  Scenario Outline: Set print size
    When I set the print size to <width> by <height> <unit> at <dpi> DPI
    Then the image is <pw> by <ph> pixels
    And the canvas DPI is <dpi>

    Examples:
      | width | height | unit   | dpi | pw  | ph  |
      | 2     | 1      | inches | 150 | 300 | 150 |
      | 2.54  | 2.54   | cm     | 100 | 100 | 100 |
