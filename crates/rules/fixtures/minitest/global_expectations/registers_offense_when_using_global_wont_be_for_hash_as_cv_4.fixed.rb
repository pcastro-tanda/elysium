it 'does something' do
  @@n = do_something
  value(@@n[:foo]).wont_be 42
end
