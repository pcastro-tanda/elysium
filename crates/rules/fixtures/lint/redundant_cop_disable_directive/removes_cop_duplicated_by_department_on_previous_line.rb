# rubocop:disable Metrics
def bar
  do_something # rubocop:disable Metrics/ClassLength
               ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Unnecessary disabling of `Metrics/ClassLength`.
end
