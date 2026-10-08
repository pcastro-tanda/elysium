it 'does something' do
  @@n = do_something
  value(@@n[:foo]).must_be_nil 42
end
