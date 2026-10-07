it 'does something' do
  @n = do_something
  @n[:foo].must_be_close_to 42
  ^^^^^^^^ Use `_(@n[:foo])` instead.
end
