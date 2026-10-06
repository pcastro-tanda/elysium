it 'does something' do
  @@n = do_something
  value(@@n).wont_be_nil 42
end
