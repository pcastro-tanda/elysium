it 'does something' do
  @n = do_something
  _(@n[:foo]).wont_be_within_epsilon 42
end
