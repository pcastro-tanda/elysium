it 'does something' do
  @n = do_something
  _(@n[:foo]).path_wont_exist 42
end
