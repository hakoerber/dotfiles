case "$TERM" in
    urxvt)
        TERM="rxvt-unicode"
        ;;
    screen)
        TERM="screen-256color"
        ;;
esac
export TERM
