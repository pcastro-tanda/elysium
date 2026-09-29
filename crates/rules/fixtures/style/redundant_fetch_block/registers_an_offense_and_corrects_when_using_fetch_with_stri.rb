# frozen_string_literal: true
hash.fetch(:key) { 'value' }
     ^^^^^^^^^^^^^^^^^^^^^^^ Use `fetch(:key, 'value')` instead of `fetch(:key) { 'value' }`.
