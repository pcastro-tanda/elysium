unless nochdir
  Dir.chdir "/"    # Release old working directory.
end

File.umask 0000    # Ensure sensible umask.
