it 'does something' do
  @n = do_something
  _(@n).wont_be 42
end
