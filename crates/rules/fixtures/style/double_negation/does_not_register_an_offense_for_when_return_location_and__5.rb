def foo?
  unless condition_foo?
    !!foo
  end
end

def bar?
  unless condition_bar?
    do_something
    !!bar
  end
end
