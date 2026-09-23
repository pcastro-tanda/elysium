def foo; end

def bar # rubocop:disable Metrics/ClassLength
        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Unnecessary disabling of `Metrics/ClassLength`.
  do_something do
  end
end
