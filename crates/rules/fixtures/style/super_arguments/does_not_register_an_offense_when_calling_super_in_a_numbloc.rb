def foo(a)
  delegate_to_define_method do
    bar(_1)
    super(a)
  end
end
