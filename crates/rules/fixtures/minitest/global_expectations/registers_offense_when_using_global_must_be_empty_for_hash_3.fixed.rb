it 'does something' do
  n = do_something
  value(n[:foo]).must_be_empty 42
end
