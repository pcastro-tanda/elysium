it 'does something' do
  @n = do_something
  _(@n[:foo]).must_be 42
end
