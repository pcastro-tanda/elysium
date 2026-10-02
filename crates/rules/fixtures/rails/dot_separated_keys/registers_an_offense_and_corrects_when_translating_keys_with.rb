I18n.t :key, scope: [:one, :two]
             ^^^^^^^^^^^^^^^^^^^ Use the dot-separated keys instead of specifying the `:scope` option.
I18n.translate :key, scope: [:one, :two]
                     ^^^^^^^^^^^^^^^^^^^ Use the dot-separated keys instead of specifying the `:scope` option.
t :key, scope: [:one, :two]
        ^^^^^^^^^^^^^^^^^^^ Use the dot-separated keys instead of specifying the `:scope` option.
translate :key, scope: [:one, :two]
                ^^^^^^^^^^^^^^^^^^^ Use the dot-separated keys instead of specifying the `:scope` option.
t :key, scope: [:one, :two], default: 'Not here'
        ^^^^^^^^^^^^^^^^^^^ Use the dot-separated keys instead of specifying the `:scope` option.
I18n.t :key, scope: ['one', :two]
             ^^^^^^^^^^^^^^^^^^^^ Use the dot-separated keys instead of specifying the `:scope` option.
I18n.t 'key', scope: [:one, :two]
              ^^^^^^^^^^^^^^^^^^^ Use the dot-separated keys instead of specifying the `:scope` option.
I18n.t :key, scope: :one
             ^^^^^^^^^^^ Use the dot-separated keys instead of specifying the `:scope` option.
I18n.t '.key', scope: :one
               ^^^^^^^^^^^ Use the dot-separated keys instead of specifying the `:scope` option.
