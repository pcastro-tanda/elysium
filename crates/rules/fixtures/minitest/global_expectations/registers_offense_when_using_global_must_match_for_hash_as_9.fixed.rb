it 'does something' do
  @@n = do_something
  _(@@n[:foo]).must_match 42
end
