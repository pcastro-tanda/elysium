it 'does something' do
  n = do_something
  value(n[:foo]).must_be_same_as 42
end
