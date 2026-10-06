it 'does something' do
  @@n = do_something
  @@n[:foo].path_wont_exist 42
  ^^^^^^^^^ Use `_(@@n[:foo])` instead.
end
