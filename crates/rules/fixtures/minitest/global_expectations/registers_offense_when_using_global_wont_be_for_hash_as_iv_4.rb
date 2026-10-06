it 'does something' do
  @n = do_something
  @n[:foo].wont_be 42
  ^^^^^^^^ Use `value(@n[:foo])` instead.
end
