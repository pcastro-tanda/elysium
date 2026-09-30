# rubocop:disable Metrics
def bar
  do_something # rubocop:disable Metrics/ClassLength -- the reason
               ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Unnecessary disabling of `Metrics/ClassLength`.
end
