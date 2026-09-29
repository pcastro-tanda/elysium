def self.some_method
  test rescue modifier_handle
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid using `rescue` in its modifier form.
rescue
  normal_handle
end
