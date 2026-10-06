it 'does something' do
  @@n = do_something
  _(@@n).path_must_exist 42
end
