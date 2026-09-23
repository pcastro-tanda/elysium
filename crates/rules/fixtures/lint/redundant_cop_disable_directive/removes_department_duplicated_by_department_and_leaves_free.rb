# rubocop:disable Metrics
def bar
  do_something # rubocop:disable Metrics - note
               ^^^^^^^^^^^^^^^^^^^^^^^^^ Unnecessary disabling of `Metrics` department.
end
