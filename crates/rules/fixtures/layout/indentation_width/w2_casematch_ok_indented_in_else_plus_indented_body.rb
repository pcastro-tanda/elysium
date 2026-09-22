case code_type
  in 'ruby' | 'sql' | 'plain'
    code_type
  in 'erb'
    'ruby; html-script: true'
  in "html"
    'xml'
  else
    'plain'
end
