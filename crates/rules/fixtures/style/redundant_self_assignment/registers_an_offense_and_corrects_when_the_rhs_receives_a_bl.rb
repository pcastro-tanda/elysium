foo = foo.delete_if { true }
    ^ Redundant self assignment detected. Method `delete_if` modifies its receiver in place.
