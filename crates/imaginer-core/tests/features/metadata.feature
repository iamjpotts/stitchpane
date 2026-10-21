Feature: Metadata

  Background:
    Given a photo "cam.jpg" of 40 by 20 pixels with EXIF
    And a "red" image "plain.png" of 40 by 20 pixels

  @META-001
  Scenario: Notice for a single image
    When I open "cam.jpg"
    Then the EXIF notice is "Contains EXIF metadata"
    When I open "plain.png"
    Then there is no EXIF notice

  @META-001
  Scenario: Notice counts images when nothing is selected
    Given a photo "cam2.jpg" of 40 by 20 pixels with EXIF
    When I open "cam.jpg", "plain.png" and "cam2.jpg" together
    Then the EXIF notice is "2 images contain EXIF metadata"
    When I select image 2
    Then there is no EXIF notice
    When I select image 1
    Then the EXIF notice is "Contains EXIF metadata"

  @META-002
  Scenario: Clicking the notice opens the viewer for the right image
    Given a photo "cam2.jpg" of 40 by 20 pixels with EXIF
    When I open "cam.jpg", "plain.png" and "cam2.jpg" together
    Then the EXIF viewer opens on image 1
    And the EXIF viewer can switch between images 1 and 3
    When I select image 3
    Then the EXIF viewer opens on image 3

  @META-003
  Scenario: The viewer groups tags
    When I open "cam.jpg"
    Then the EXIF viewer shows "Manufacturer of image input equipment" in group "Camera"
    And the EXIF viewer shows "Exposure time" in group "Capture settings"
    And the EXIF viewer shows "Date and time of original data generation" in group "Date and time"
    And the EXIF viewer shows "Latitude" in group "Location"
    And the EXIF viewer shows "Software used" in group "Other"

  @META-004
  Scenario: Strip metadata
    When I open "cam.jpg"
    And I strip image 1's metadata
    Then there is no EXIF notice
    And the undo label is "Undo Strip metadata"
    And "cam.jpg" is unchanged on disk
    When I press "Ctrl+Z"
    Then the EXIF notice is "Contains EXIF metadata"

  @META-005
  Scenario: Default export metadata for one image
    When I open "cam.jpg"
    And I prepare to export to "out.jpg"
    Then the export metadata comes from image 1
    And the export metadata groups are camera, capture settings, date and time, and other
    When I export to "out.jpg" with the prepared settings
    Then "out.jpg" has EXIF "Make"
    And "out.jpg" has no EXIF "GPSLatitude"
    And "out.jpg" has EXIF orientation 1 and dimensions 40 by 20

  @META-005
  Scenario: Compositions export no metadata by default
    When I open "cam.jpg" and "plain.png" together
    And I prepare to export to "out.jpg"
    Then the export metadata comes from no image

  @META-005
  Scenario: Choose the image and groups
    When I open "cam.jpg" and "plain.png" together
    And I prepare to export to "out.png"
    And I choose export metadata from image 1 with only the location group
    And I export to "out.png" with the prepared settings
    Then "out.png" has EXIF "GPSLatitude"
    And "out.png" has no EXIF "Make"

  @META-005
  Scenario: Stripped images are not offered
    When I open "cam.jpg"
    And I strip image 1's metadata
    And I prepare to export to "out.jpg"
    Then the export metadata comes from no image

  @META-006
  Scenario: EXIF orientation is applied on load
    Given a patterned photo "turned.jpg" of 40 by 20 pixels with EXIF orientation 6
    When I open "turned.jpg"
    Then the canvas is 20 by 40 pixels
    And the rendered canvas is roughly the source rotated a quarter turn clockwise
    And the project has 1 steps
