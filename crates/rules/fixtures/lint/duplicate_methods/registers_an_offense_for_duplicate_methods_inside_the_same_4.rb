foo { Class.new(_1) do
    def foo
      1
    end
    def foo
    ^^^^^^^ Method `Object#foo` is defined at both (string):2 and (string):5.
      2
    end
  end
}
