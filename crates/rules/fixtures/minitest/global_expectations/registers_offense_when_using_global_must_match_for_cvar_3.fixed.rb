it 'does something' do
  @@n = do_something
  value(@@n).must_match 42
end
