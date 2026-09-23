class One
  # rubocop:disable Style/ClassVars
  @@class_var = 1  # offense here
end

class Two
  # rubocop:disable Style/ClassVars
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Unnecessary disabling of `Style/ClassVars`.
  @@class_var = 2  # offense and here
  # rubocop:enable Style/ClassVars
end
