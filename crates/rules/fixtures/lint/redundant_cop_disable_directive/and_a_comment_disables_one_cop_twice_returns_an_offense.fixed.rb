class One
  # rubocop:disable Style/ClassVars
  @@class_var = 1  # offense here
end

class Two
  @@class_var = 2  # offense and here
  # rubocop:enable Style/ClassVars
end
