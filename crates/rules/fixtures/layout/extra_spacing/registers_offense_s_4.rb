unless nochdir
  Dir.chdir "/"    # Release old working directory.
               ^^^ Unnecessary spacing detected.
end

File.umask 0000    # Ensure sensible umask.
               ^^^ Unnecessary spacing detected.
