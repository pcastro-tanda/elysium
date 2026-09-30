def foo(obj)
  obj.do_qux do
    def bar
    ^^^^^^^ Method definitions must not be nested. Use `lambda` instead.
    end
  end
end
