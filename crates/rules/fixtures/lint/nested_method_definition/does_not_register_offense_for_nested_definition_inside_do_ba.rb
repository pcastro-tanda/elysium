def foo(obj)
  obj.do_baz do
    def bar
    end
  end
end
