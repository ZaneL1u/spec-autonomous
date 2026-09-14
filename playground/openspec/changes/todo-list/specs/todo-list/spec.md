## ADDED Requirements

### Requirement: Add a todo item
The todo module SHALL add a non-empty item and return its stable numeric position.

#### Scenario: Add item
- **WHEN** an item is added
- **THEN** the item is returned with its position

### Requirement: List todo items
The todo module SHALL list items in insertion order.

#### Scenario: Empty list
- **WHEN** no items exist
- **THEN** listing returns an empty array
