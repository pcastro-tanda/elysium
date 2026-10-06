it 'does something' do
  @n = do_something
  @n[:foo].must_be_kind_of 42
  ^^^^^^^^ Use `value(@n[:foo])` instead.
end
