def foo(a)
  delegate_to_define_method do
    super(a)
  end
end
