create table app_info
(
    label TEXT    not null,
    value integer not null
);

create table base
(
    id         integer              not null
        constraint base_pk_2
            primary key autoincrement,
    label      TEXT                 not null
        constraint base_pk
            unique,
    is_checked integer default true not null
);

create table item
(
    id         integer              not null
        constraint item_pk
            primary key autoincrement,
    path       TEXT                 not null,
    base_id    integer              not null
        constraint item_base_id_fk
            references base
            on update cascade on delete cascade,
    hash       TEXT    default ''   not null,
    is_checked integer default true not null,
    parent     integer
        constraint item_item_id_fk
            references item
            on update cascade on delete cascade,
    name       TEXT    default ''   not null,
    note       TEXT    default ''   not null,
    created_at INTEGER,
    updated_at integer,
    model_name TEXT    default ''   not null,
    constraint item_pk_2
        unique (path, base_id),
    constraint item_pk_3
        unique (path, parent)
);

create table tag
(
    name        TEXT not null
        constraint tag_pk
            primary key,
    description integer
);

create table tag_item
(
    tag  TEXT    not null
        constraint tag_item_tag_id_fk
            references tag
            on update cascade on delete cascade,
    item integer not null
        constraint tag_model_model_id_fk
            references item
            on update cascade on delete cascade,
    constraint tag_item_pk
        primary key (tag, item)
);

create unique index tag_item_item_tag_uindex
    on tag_item (item, tag);

create table tag_tag
(
    tag    TEXT not null
        constraint tag_tag_tag_id_fk
            references tag
            on update cascade on delete cascade,
    depend TEXT not null
        constraint tag_tag_tag_id_fk_2
            references tag
            on update cascade on delete cascade,
    constraint tag_tag_pk
        primary key (tag, depend)
);


